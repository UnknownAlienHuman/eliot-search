//! One profile handshake for standalone native and existing low-level callers.

use std::net::TcpStream;

use search_contracts::ProtocolVersion;
use search_ports::CancellationProbe;
use search_provider_protocol::{
    BoundSession, MonotonicMillis, ProofDigest, ProtocolError, TypedTransportProfileV1, verify_proof,
};

use super::{CanonicalProviderConnection, CanonicalTcpConnection, CanonicalTcpError, TcpState, io, monotonic_millis};
use io::SocketIo;

impl CanonicalProviderConnection {
    /// Takes the ORIGINAL paired, unbuffered loopback socket into typed mode.
    ///
    /// This low-level entry retains its existing contract: the caller owns
    /// native registration/lifetime validation. Production standalone bootstrap
    /// uses open_standalone_tcp so that validation also brackets actual I/O.
    /// No other reader, socket clone or unread legacy buffer may remain.
    /// The exact profile offer and separate direction-bound proofs are unchanged.
    /// Failure or unwind closes the socket/session; no fallback or retry occurs.
    pub fn into_tcp(
        self,
        stream: TcpStream,
        setup_deadline_ms: u64,
    ) -> Result<CanonicalTcpConnection, CanonicalTcpError> {
        let started = monotonic_millis();
        let deadline = io::deadline(started, setup_deadline_ms)?;
        self.into_tcp_authorized(stream, started, deadline, None, |_| Ok(deadline))
    }

    // Caller supplies an ABSOLUTE original deadline, never another relative
    // timeout after native reads. Authorization can only tighten the budget.
    // The callback borrows the actual session, not a caller-selected identity.
    // Keep the native lock across this entire method. An error, including after
    // acknowledgement bytes were written, drops the local transport and its key.
    pub(crate) fn into_tcp_authorized<E>(
        self,
        stream: TcpStream,
        started: MonotonicMillis,
        deadline: MonotonicMillis,
        cancellation: Option<&dyn CancellationProbe>,
        mut authorize: impl FnMut(&BoundSession) -> Result<MonotonicMillis, E>,
    ) -> Result<CanonicalTcpConnection, E>
    where
        E: From<CanonicalTcpError>,
    {
        let mut transport = CanonicalTcpConnection {
            state: Some(TcpState { connection: self, io: SocketIo::new(stream) }),
        };
        let state = transport.state.as_mut().ok_or(CanonicalTcpError::Closed)?;
        check(started, deadline, cancellation)?;
        if !state.connection.session.is_active() {
            return Err(CanonicalTcpError::Closed.into());
        }
        if state.connection.session.binding_context().version() != (ProtocolVersion { major: 1, minor: 0 }) {
            return Err(CanonicalTcpError::Protocol(ProtocolError::NoCompatibleVersion).into());
        }
        state.io.configure()?;
        let mut deadline = deadline.min(authorize(&state.connection.session)?);
        check(started, deadline, cancellation)?;
        let mut preface = [0_u8; TypedTransportProfileV1::PREFACE.len()];
        state.io.read_exact(&mut preface, deadline, cancellation)?;
        if preface.as_slice() != TypedTransportProfileV1::PREFACE {
            return Err(CanonicalTcpError::ProfileMismatch.into());
        }
        let mut proof = [0_u8; TypedTransportProfileV1::PROOF_BYTES];
        state.io.read_exact(&mut proof, deadline, cancellation)?;
        let ceremony = state.connection.session.pairing().session();
        let nonce = *state.connection.session.server_nonce();
        let hash = |parts: [&[u8]; 4]| state.connection.key.with_bytes(|key| {
            let mut hasher = blake3::Hasher::new_keyed(key);
            for part in parts { hasher.update(part); }
            ProofDigest::from_bytes(*hasher.finalize().as_bytes())
        });
        let expected = hash(TypedTransportProfileV1::offer_transcript(&ceremony, &nonce));
        if !verify_proof(&expected, &ProofDigest::from_bytes(proof)) {
            return Err(CanonicalTcpError::Protocol(ProtocolError::AuthenticationFailed).into());
        }
        let accepted = hash(TypedTransportProfileV1::accept_transcript(&ceremony, &nonce));
        // No acknowledgement byte may precede this fresh native validation.
        // It catches expiry during a slow/partial offer, including UTC jumps.
        deadline = deadline.min(authorize(&state.connection.session)?);
        check(started, deadline, cancellation)?;
        state.io.write_parts(
            &[TypedTransportProfileV1::PREFACE, accepted.as_bytes()], deadline, cancellation,
        )?;
        deadline = deadline.min(authorize(&state.connection.session)?);
        check(started, deadline, cancellation)?;
        Ok(transport)
    }
}

fn check(
    started: MonotonicMillis,
    deadline: MonotonicMillis,
    cancellation: Option<&dyn CancellationProbe>,
) -> Result<(), CanonicalTcpError> {
    if cancellation.is_some_and(CancellationProbe::is_cancelled) {
        return Err(CanonicalTcpError::Cancelled);
    }
    let now = monotonic_millis();
    if now < started || now >= deadline {
        return Err(CanonicalTcpError::DeadlineExpired);
    }
    Ok(())
}
