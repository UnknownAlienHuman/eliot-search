//! Typed local-provider client for canonical authenticated P00 connections.
//!
//! The explicit TCP handoff is transitional compatibility code. Native pairing,
//! typed-profile negotiation and request/session I/O share one transport-neutral
//! stream owner so the final per-installation named-pipe adapter cannot create a
//! second protocol stack or fallback route.

mod io;
mod local;
mod native;
mod state;

pub use native::NativeClientBinding;

use std::net::TcpStream;
use std::task::Poll;
use std::time::Duration;

use search_contracts::{
    InstallationId, ProtocolFailureCode, ProviderEnvelope, RequestBody, RequestId,
};
use search_provider_protocol::{
    BindingContext, BindingKey, PairingMachine, ProofDigest, ProtocolError,
    ProtocolLimits, RequestStatus, SessionMachine, TypedTransportProfileV1,
    verify_proof,
};

use io::{SetupBudget, SocketIo};
use state::State;

/// Local transport or protocol failure; authenticated remote error bodies remain
/// typed `ProviderEnvelope` values, not a successful command or fabricated code.
pub enum TypedClientError {
    /// Shared protocol validation failed.
    Protocol(ProtocolError),
    /// A local transport operation failed; raw OS errors are not retained.
    Io,
    /// The original setup or pending request budget expired.
    DeadlineExpired,
    /// The process-local cancellation capability interrupted setup.
    Cancelled,
    /// The peer closed before a complete record arrived.
    PeerClosed,
    /// A non-empty transport write made no progress.
    WriteZero,
    /// The server did not acknowledge the exact typed/MAC profile.
    ProfileMismatch,
    /// A transitional TCP adapter observed a non-loopback endpoint.
    NonLoopback,
    /// A bounded allocation could not be reserved.
    Allocation,
    /// No request or cancellation acknowledgement is outstanding.
    NothingPending,
    /// This client already has one outstanding cancellation control.
    CancellationPending,
    /// The mandatory initial standalone grant has not been issued.
    GrantRequired,
    /// This session already spent the one initial standalone-grant command.
    GrantAlreadyIssued,
    /// A recipe attempted to substitute claims other than the exact issued grant.
    GrantMismatch,
    /// The authenticated daemon refused the initial grant request.
    GrantRejected {
        /// Closed terminal status authenticated by the daemon.
        status: RequestStatus,
        /// Public content-free failure code authenticated in the response body.
        failure: ProtocolFailureCode,
    },
    /// The response does not belong to an expected request/recipe/event.
    ResponseMismatch,
    /// The connection has already closed and cannot be reused.
    Closed,
}

impl TypedClientError {
    /// Content-free reason; never includes queries, handle tokens or proofs.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Protocol(error) => error.code(),
            Self::Io => "REMOTE_TYPED_IO_ERROR",
            Self::DeadlineExpired => "REMOTE_DEADLINE_EXPIRED",
            Self::Cancelled => "REMOTE_TYPED_CANCELLED",
            Self::PeerClosed => "REMOTE_TYPED_FRAME_TRUNCATED",
            Self::WriteZero => "REMOTE_TYPED_WRITE_ZERO",
            Self::ProfileMismatch => "REMOTE_TYPED_PROFILE_MISMATCH",
            Self::NonLoopback => "REMOTE_TYPED_NOT_LOOPBACK",
            Self::Allocation => "REMOTE_TYPED_RESOURCE_EXHAUSTED",
            Self::NothingPending => "REMOTE_TYPED_NOTHING_PENDING",
            Self::CancellationPending => "REMOTE_TYPED_CANCEL_PENDING",
            Self::GrantRequired => "REMOTE_GRANT_REQUIRED",
            Self::GrantAlreadyIssued => "REMOTE_GRANT_ALREADY_ISSUED",
            Self::GrantMismatch => "REMOTE_GRANT_MISMATCH",
            Self::GrantRejected { status, .. } => match status {
                RequestStatus::Cancelled => "REMOTE_GRANT_CANCELLED",
                RequestStatus::OutcomeUnknown => "REMOTE_GRANT_OUTCOME_UNKNOWN",
                RequestStatus::Ok | RequestStatus::Partial | RequestStatus::Failed => {
                    "REMOTE_GRANT_REJECTED"
                }
            },
            Self::ResponseMismatch => "REMOTE_RESPONSE_MISMATCH",
            Self::Closed => "REMOTE_SESSION_CLOSED",
        }
    }
}

impl From<ProtocolError> for TypedClientError {
    fn from(error: ProtocolError) -> Self { Self::Protocol(error) }
}

impl From<std::io::Error> for TypedClientError {
    fn from(_error: std::io::Error) -> Self { Self::Io }
}

impl std::fmt::Debug for TypedClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("TypedClientError")
            .field(&self.code())
            .finish()
    }
}

impl std::fmt::Display for TypedClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for TypedClientError {}

#[cfg(test)]
mod diagnostic_tests {
    use std::error::Error as _;

    use super::TypedClientError;

    #[test]
    fn arbitrary_os_error_text_cannot_escape_through_public_diagnostics() {
        let error = TypedClientError::from(std::io::Error::other(
            "private credential locator and query sentinel",
        ));
        assert_eq!(error.code(), "REMOTE_TYPED_IO_ERROR");
        assert_eq!(error.to_string(), "REMOTE_TYPED_IO_ERROR");
        assert_eq!(format!("{error:?}"), "TypedClientError(\"REMOTE_TYPED_IO_ERROR\")");
        assert!(error.source().is_none());
    }
}

/// Sole owner of one typed local stream/key and its bounded client lifecycle.
///
/// Sending and receiving are separate, so a caller can send a cancel while a
/// recipe is outstanding. Up to the negotiated work limit and one additional
/// cancel may await replies. The cancel reserves no work slot. Reads are
/// synchronous, not background tasks; no reader threads or stream clones exist.
/// Failure/unwind drops the stream and all state. There is no reconnect,
/// operation replay, deadline renewal, legacy fallback or implicit daemon shutdown.
/// Received values are authenticated protocol data, not current access permits.
pub struct TypedProviderSession {
    state: Option<State>,
}

impl TypedProviderSession {
    /// Negotiate the typed profile on an already-paired compatibility TCP stream.
    ///
    /// The handoff must leave no other reader or prefetched bytes behind. The
    /// binding must match the completed ceremony and the key must reproduce its
    /// provider proof. The server must then acknowledge the exact profile under
    /// a separate proof domain. No development token is promoted to authority.
    /// A single finite deadline covers these checks and both I/O directions.
    pub fn from_paired(
        stream: TcpStream,
        binding: BindingContext,
        ceremony: PairingMachine,
        key: BindingKey,
        nonce: search_provider_protocol::ServerNonce,
        limits: ProtocolLimits,
        setup_timeout: Duration,
    ) -> Result<Self, TypedClientError> {
        let socket = SocketIo::new(stream);
        let never_cancelled = || false;
        let budget = SetupBudget::new(setup_timeout, &never_cancelled)?;
        Self::from_paired_socket(socket, binding, ceremony, key, nonce, limits, &budget)
    }

    pub(super) fn from_paired_socket(
        mut socket: SocketIo,
        binding: BindingContext,
        ceremony: PairingMachine,
        key: BindingKey,
        nonce: search_provider_protocol::ServerNonce,
        limits: ProtocolLimits,
        budget: &SetupBudget<'_>,
    ) -> Result<Self, TypedClientError> {
        budget.remaining()?;
        let limits = limits.validate()?;
        let transcript = ceremony.server_transcript()?;
        let pairing = ceremony.into_verified()?;
        binding.verify_pairing(&pairing)?;
        let expected = key.with_bytes(|bytes| {
            ProofDigest::from_bytes(*blake3::keyed_hash(bytes, transcript.as_bytes()).as_bytes())
        });
        if !verify_proof(&expected, &pairing.provider_proof()) {
            return Err(ProtocolError::AuthenticationFailed.into());
        }
        if binding.version() != (search_contracts::ProtocolVersion { major: 1, minor: 0 }) {
            return Err(ProtocolError::NoCompatibleVersion.into());
        }
        socket.configure()?;
        budget.remaining()?;
        let ceremony_id = pairing.session();
        let offer = keyed_parts(
            &key,
            TypedTransportProfileV1::offer_transcript(&ceremony_id, &nonce),
        );
        socket.write_parts_setup(
            &[TypedTransportProfileV1::PREFACE, offer.as_bytes()],
            budget,
        )?;
        let mut preface = [0_u8; TypedTransportProfileV1::PREFACE.len()];
        socket.read_exact_setup(&mut preface, budget)?;
        if preface.as_slice() != TypedTransportProfileV1::PREFACE {
            return Err(TypedClientError::ProfileMismatch);
        }
        let mut observed = [0_u8; TypedTransportProfileV1::PROOF_BYTES];
        socket.read_exact_setup(&mut observed, budget)?;
        let accepted = keyed_parts(
            &key,
            TypedTransportProfileV1::accept_transcript(&ceremony_id, &nonce),
        );
        if !verify_proof(&accepted, &ProofDigest::from_bytes(observed)) {
            return Err(ProtocolError::AuthenticationFailed.into());
        }
        let mut session = SessionMachine::new(limits, 1, 1)?;
        session.negotiate(binding.version())?;
        session.activate(&accepted, &ProofDigest::from_bytes(observed))?;
        let state = State::new(socket, binding, pairing, key, nonce, limits, session);
        budget.remaining()?;
        Ok(Self { state: Some(state) })
    }

    /// Bind one independently trusted installation identity before grant issuance.
    ///
    /// Rebinding to a different installation fails closed and drops the session.
    pub(super) fn bind_trusted_installation_id(
        &mut self,
        installation_id: InstallationId,
    ) -> Result<(), TypedClientError> {
        self.with_state(|state| state.bind_trusted_installation_id(installation_id))
    }

    /// Send an existing typed recipe and the exact server-issued grant unchanged.
    pub fn send_request(
        &mut self,
        body: RequestBody,
        timeout: Duration,
    ) -> Result<RequestId, TypedClientError> {
        self.with_state(|state| state.send_request(body, timeout))
    }

    /// Send a fresh cancellation control identity for one target request.
    pub fn send_cancel(
        &mut self,
        control_id: RequestId,
        target: RequestId,
        timeout: Duration,
    ) -> Result<(), TypedClientError> {
        self.with_state(|state| state.send_cancel(control_id, target, timeout))
    }

    /// Return one whole authenticated, correlated typed event; print nothing.
    pub fn receive(&mut self) -> Result<ProviderEnvelope, TypedClientError> {
        self.with_state(State::receive)
    }

    /// Poll one response without holding the caller until a whole frame arrives.
    pub fn poll_receive(
        &mut self,
        quantum: Duration,
    ) -> Result<Poll<ProviderEnvelope>, TypedClientError> {
        self.with_state(|state| state.poll_receive(quantum))
    }

    /// Drop key, client metadata and the sole local stream, idempotently.
    ///
    /// This sends no shutdown request and does not claim to undo server effects.
    pub fn close(&mut self) { drop(self.state.take()); }

    fn with_state<R>(
        &mut self,
        action: impl FnOnce(&mut State) -> Result<R, TypedClientError>,
    ) -> Result<R, TypedClientError> {
        let mut operation = Operation { state: &mut self.state, completed: false };
        let state = operation.state.as_mut().ok_or(TypedClientError::Closed)?;
        let result = action(state);
        operation.completed = result.is_ok();
        result
    }
}

impl Drop for TypedProviderSession {
    fn drop(&mut self) { self.close(); }
}

struct Operation<'a> {
    state: &'a mut Option<State>,
    completed: bool,
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        if !self.completed { drop(self.state.take()); }
    }
}

fn keyed_parts(key: &BindingKey, parts: [&[u8]; 4]) -> ProofDigest {
    key.with_bytes(|bytes| {
        let mut hasher = blake3::Hasher::new_keyed(bytes);
        for part in parts { hasher.update(part); }
        ProofDigest::from_bytes(*hasher.finalize().as_bytes())
    })
}
