//! Typed TCP client for an explicitly handed-off, authenticated P00 connection.
//!
//! This is not an upgrade of the development token-file session. Bootstrap must
//! supply the original socket, live binding context, completed ceremony, key and
//! server nonce. No grants are minted and no store or secret file is opened here.

mod io;
mod state;

use std::net::TcpStream;
use std::task::Poll;
use std::time::Duration;

use search_contracts::{ProviderEnvelope, RequestBody, RequestId};
use search_provider_protocol::{
    BindingContext, BindingKey, PairingMachine, ProofDigest, ProtocolError,
    ProtocolLimits, SessionMachine, TypedTransportProfileV1, verify_proof,
};

use io::{SocketIo, budget};
use state::State;

/// Local transport or protocol failure; authenticated remote error bodies remain
/// typed ProviderEnvelope values, not a successful command or a fabricated code.
#[derive(Debug)]
pub enum TypedClientError {
    /// Shared protocol validation failed.
    Protocol(ProtocolError),
    /// A socket operation failed.
    Io(std::io::Error),
    /// The original setup or pending request budget expired.
    DeadlineExpired,
    /// The peer closed before a complete record arrived.
    PeerClosed,
    /// A non-empty socket write made no progress.
    WriteZero,
    /// The server did not acknowledge the exact typed/MAC profile.
    ProfileMismatch,
    /// Both socket endpoints must be local loopback addresses.
    NonLoopback,
    /// A bounded allocation could not be reserved.
    Allocation,
    /// No request or cancellation acknowledgement is outstanding.
    NothingPending,
    /// This client already has one outstanding cancellation control.
    CancellationPending,
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
            Self::Io(_) => "REMOTE_TYPED_IO_ERROR",
            Self::DeadlineExpired => "REMOTE_DEADLINE_EXPIRED",
            Self::PeerClosed => "REMOTE_TYPED_FRAME_TRUNCATED",
            Self::WriteZero => "REMOTE_TYPED_WRITE_ZERO",
            Self::ProfileMismatch => "REMOTE_TYPED_PROFILE_MISMATCH",
            Self::NonLoopback => "REMOTE_TYPED_NOT_LOOPBACK",
            Self::Allocation => "REMOTE_TYPED_RESOURCE_EXHAUSTED",
            Self::NothingPending => "REMOTE_TYPED_NOTHING_PENDING",
            Self::CancellationPending => "REMOTE_TYPED_CANCEL_PENDING",
            Self::ResponseMismatch => "REMOTE_RESPONSE_MISMATCH",
            Self::Closed => "REMOTE_SESSION_CLOSED",
        }
    }
}

impl From<ProtocolError> for TypedClientError {
    fn from(error: ProtocolError) -> Self { Self::Protocol(error) }
}

impl From<std::io::Error> for TypedClientError {
    fn from(error: std::io::Error) -> Self { Self::Io(error) }
}

impl std::fmt::Display for TypedClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.code()) }
}

impl std::error::Error for TypedClientError {}

/// Sole owner of one typed socket/key and its bounded client lifecycle.
///
/// Sending and receiving are separate, so a caller can send a cancel while a
/// recipe is outstanding. Up to the negotiated work limit and one additional
/// cancel may await replies. The cancel reserves no work slot. Reads are
/// synchronous, not background tasks; no reader threads or socket clones exist.
/// Failure/unwind closes the socket and drops all state. There is no reconnect,
/// operation replay, deadline renewal, legacy fallback or implicit daemon shutdown.
/// Received values are authenticated protocol data, not current access permits.
pub struct TypedProviderSession {
    state: Option<State>,
}

impl TypedProviderSession {
    /// Negotiates the typed profile on the original, already-paired socket.
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
        // Own teardown before the first fallible validation or socket option.
        let mut socket = SocketIo::new(stream);
        let (deadline, _) = budget(setup_timeout)?;
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
        let ceremony_id = pairing.session();
        let offer = keyed_parts(&key, TypedTransportProfileV1::offer_transcript(&ceremony_id, &nonce));
        socket.write_parts(&[TypedTransportProfileV1::PREFACE, offer.as_bytes()], deadline)?;
        let mut preface = [0_u8; TypedTransportProfileV1::PREFACE.len()];
        socket.read_exact(&mut preface, deadline)?;
        if preface.as_slice() != TypedTransportProfileV1::PREFACE {
            return Err(TypedClientError::ProfileMismatch);
        }
        let mut observed = [0_u8; TypedTransportProfileV1::PROOF_BYTES];
        socket.read_exact(&mut observed, deadline)?;
        let accepted = keyed_parts(&key, TypedTransportProfileV1::accept_transcript(&ceremony_id, &nonce));
        if !verify_proof(&accepted, &ProofDigest::from_bytes(observed)) {
            return Err(ProtocolError::AuthenticationFailed.into());
        }
        let mut session = SessionMachine::new(limits, 1, 1)?;
        session.negotiate(binding.version())?;
        session.activate(&accepted, &ProofDigest::from_bytes(observed))?;
        let state = State::new(socket, binding, pairing, key, nonce, limits, session);
        io::remaining(deadline)?;
        Ok(Self { state: Some(state) })
    }

    /// Sends an existing typed recipe and server-issued claims without editing
    /// either. The request ID comes from the recipe and cannot be reused on this
    /// connection. Success means local send completion, not server admission,
    /// authorization or recipe success. Encoding/proof/write share one budget.
    pub fn send_request(&mut self, body: RequestBody, timeout: Duration) -> Result<RequestId, TypedClientError> {
        self.with_state(|state| state.send_request(body, timeout))
    }

    /// Sends a fresh control identity for the target without completing it.
    /// A new ID may repeat a target; a repeated control ID or self-target fails.
    /// One cancel can await acknowledgement even when every work slot is full.
    /// Neither sending nor receiving its acknowledgement renews target deadlines.
    pub fn send_cancel(
        &mut self,
        control_id: RequestId,
        target: RequestId,
        timeout: Duration,
    ) -> Result<(), TypedClientError> {
        self.with_state(|state| state.send_cancel(control_id, target, timeout))
    }

    /// Returns one whole authenticated, correlated typed event; prints nothing.
    /// Checks the MAC before decoding, both directional and per-request ordering,
    /// recipe/plan identity and the original deadlines. A cancel acknowledgement
    /// belongs to its own control ID and does not remove the target request.
    /// Even a target-terminal flag is not proof of rollback or a result payload.
    /// Remote errors, ambiguity and partial coverage remain intact for rendering.
    pub fn receive(&mut self) -> Result<ProviderEnvelope, TypedClientError> {
        self.with_state(State::receive)
    }

    /// Polls one response without holding the caller until a whole frame arrives.
    ///
    /// Pending retains all partial prefix/body/MAC bytes and the original request
    /// deadline. The caller may send_cancel or close before polling again. Input
    /// waiting is capped at min(quantum, 25 ms) and 64 KiB per turn; complete-frame
    /// authentication/decoding still runs synchronously under the original deadline.
    /// Zero quantum is invalid. This is explicit polling, not an async Waker API.
    /// Only Ready exposes a fully verified event. Pending spends no sequence or
    /// request state; EOF, real deadline expiry and protocol errors still close.
    pub fn poll_receive(&mut self, quantum: Duration) -> Result<Poll<ProviderEnvelope>, TypedClientError> {
        self.with_state(|state| state.poll_receive(quantum))
    }

    /// Drops key and client metadata and shuts down the socket, idempotently.
    /// Does not send shutdown or claim to undo server effects.
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
