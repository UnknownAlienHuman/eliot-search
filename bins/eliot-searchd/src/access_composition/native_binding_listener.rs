//! Loopback accept lifetime tied to the bootstrapped standalone process owner.
//!
//! This module binds only after durable bootstrap has produced
//! [`StandaloneBootstrapReady`]. Pairing uses the canonical bounded prelude over
//! a restricted exact-I/O view of the original socket: it cannot clone, buffer
//! ahead or detach the descriptor. An opened or serving connection borrows the
//! process owner, so its transport, tasks and terminal cleanup cannot outlive the
//! control journal, admission snapshot or data-root lock.

mod connection;
mod io;
mod pairing;

pub use connection::{StandaloneOpenedConnection, StandaloneServingConnection};
pub use io::{StandalonePairingIo, StandalonePairingIoError};
pub use pairing::StandaloneNativePairingError;

use core::fmt;
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::task::Poll;
use std::time::{Duration, Instant};

use search_contracts::{OpaqueId, ProtocolRange, protocol::PeerRole};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    BindingContext, BindingSession, PairingMachine, ProtocolError, ProtocolLimits, ServerNonce,
};

use super::{NativeTcpOpenError, StandaloneBootstrapReady, StandaloneProcessOwner};

const ACCEPT_SLEEP: Duration = Duration::from_millis(2);
const MAX_ACCEPT_QUANTUM: Duration = Duration::from_millis(250);

/// Pairing or native-open failure for one accepted socket.
#[derive(Debug)]
pub enum StandaloneAcceptError {
    /// Listener or restricted pairing I/O failed.
    Io(StandalonePairingIoError),
    /// Canonical native mutual pairing refused the peer.
    Pairing(StandaloneNativePairingError),
    /// Published binding, credential, policy or typed profile opening failed.
    Open(NativeTcpOpenError),
}

impl fmt::Display for StandaloneAcceptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => fmt::Display::fmt(error, formatter),
            Self::Pairing(error) => fmt::Display::fmt(error, formatter),
            Self::Open(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for StandaloneAcceptError {}

impl From<StandalonePairingIoError> for StandaloneAcceptError {
    fn from(error: StandalonePairingIoError) -> Self { Self::Io(error) }
}

impl From<StandaloneNativePairingError> for StandaloneAcceptError {
    fn from(error: StandaloneNativePairingError) -> Self { Self::Pairing(error) }
}

impl From<NativeTcpOpenError> for StandaloneAcceptError {
    fn from(error: NativeTcpOpenError) -> Self { Self::Open(error) }
}

/// Completed mutual pairing material for the same original accepted socket.
///
/// Construction verifies that the binding and completed ceremony correspond.
/// It does not validate current durable registration, credential presence,
/// policy, source scope or recipe authority; `StandaloneProcessOwner` does that
/// again while opening the typed transport.
pub struct CompletedStandalonePairing {
    binding: BindingContext,
    ceremony: PairingMachine,
    server_nonce: ServerNonce,
}

impl CompletedStandalonePairing {
    /// Retain one mutually verified standalone ceremony.
    pub fn new(
        binding: BindingContext,
        ceremony: PairingMachine,
        server_nonce: ServerNonce,
    ) -> Result<Self, ProtocolError> {
        let verified = ceremony.clone().into_verified()?;
        binding.verify_pairing(&verified)?;
        if binding.role() != PeerRole::StandaloneCli {
            return Err(ProtocolError::AuthenticationFailed);
        }
        Ok(Self { binding, ceremony, server_nonce })
    }
}

impl fmt::Debug for CompletedStandalonePairing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedStandalonePairing")
            .field("binding", &self.binding.binding_id())
            .field("version", &self.binding.version())
            .finish_non_exhaustive()
    }
}

/// Listener plus root-owned process state. The listener is created only after
/// registration bootstrap/finalization succeeded. Field order closes admission
/// before dropping the process owner and its root lock.
pub struct StandaloneLoopbackOwner {
    listener: TcpListener,
    pairing: BindingSession,
    process: StandaloneProcessOwner,
    next_connection_sequence: u64,
    accept_quantum: Duration,
    local_protocols: ProtocolRange,
    limits: ProtocolLimits,
}

impl StandaloneLoopbackOwner {
    /// Bind an IPv4 loopback listener after process bootstrap.
    ///
    /// Port zero is permitted for an explicitly supervised ephemeral endpoint.
    /// The finite poll quantum is not a request deadline; each accepted socket
    /// receives its own caller-supplied `OperationContext` before pairing begins.
    /// The protocol range and limits are fixed for this listener lifetime.
    pub fn bind(
        process: StandaloneProcessOwner,
        port: u16,
        accept_quantum: Duration,
        local_protocols: ProtocolRange,
        limits: ProtocolLimits,
    ) -> Result<Self, StandalonePairingIoError> {
        if accept_quantum.is_zero() || accept_quantum > MAX_ACCEPT_QUANTUM {
            return Err(StandalonePairingIoError::InvalidConfiguration);
        }
        let local_protocols = ProtocolRange::new(
            local_protocols.minimum,
            local_protocols.maximum,
        )
        .map_err(|_| StandalonePairingIoError::InvalidConfiguration)?;
        let limits = limits
            .validate()
            .map_err(|_| StandalonePairingIoError::InvalidConfiguration)?;
        let pairing = BindingSession::new(local_protocols, limits)
            .map_err(|_| StandalonePairingIoError::InvalidConfiguration)?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .map_err(StandalonePairingIoError::Io)?;
        if !listener
            .local_addr()
            .map_err(StandalonePairingIoError::Io)?
            .ip()
            .is_loopback()
        {
            return Err(StandalonePairingIoError::NonLoopback);
        }
        listener
            .set_nonblocking(true)
            .map_err(StandalonePairingIoError::Io)?;
        Ok(Self {
            listener,
            pairing,
            process,
            next_connection_sequence: 0,
            accept_quantum,
            local_protocols,
            limits,
        })
    }

    /// Actual bound address; contains no credential or source metadata.
    pub fn local_addr(&self) -> Result<SocketAddr, StandalonePairingIoError> {
        self.listener.local_addr().map_err(StandalonePairingIoError::Io)
    }

    /// Executed bootstrap evidence retained by the underlying process owner.
    #[must_use]
    pub const fn readiness(&self) -> &StandaloneBootstrapReady {
        self.process.readiness()
    }

    /// Poll one accepted socket, complete canonical native mutual pairing on the
    /// original descriptor, then perform native registration/policy checks and
    /// typed transport negotiation.
    ///
    /// `Poll::Pending` means no connection arrived during the configured accept
    /// quantum; no setup deadline or connection sequence was consumed. A failed
    /// accepted connection is closed and never retried or routed to a legacy
    /// protocol.
    pub fn poll_open<C: CancellationProbe + Clone>(
        &mut self,
        boot_id: &OpaqueId,
        context: &OperationContext<C>,
    ) -> Result<Poll<StandaloneOpenedConnection<'_>>, StandaloneAcceptError> {
        let Some(mut stream) = self.poll_accept()? else {
            return Ok(Poll::Pending);
        };
        let sequence = self
            .next_connection_sequence
            .checked_add(1)
            .ok_or(StandalonePairingIoError::ConnectionSequenceExhausted)?;
        self.next_connection_sequence = sequence;

        let record = self.process.readiness().record().clone();
        let expected = self.process.readiness().expectation().clone();
        let mut io = StandalonePairingIo::new(&mut stream, context)?;
        let paired = pairing::perform_native_pairing(
            &mut io,
            sequence,
            &mut self.pairing,
            self.local_protocols,
            self.limits,
            &record,
            &expected,
        )?;
        let remaining = io.remaining_context()?;
        drop(io);

        let (transport, pin) = self.process.open_tcp(
            stream,
            paired.binding,
            paired.ceremony,
            paired.server_nonce,
            self.limits,
            boot_id,
            &remaining,
        )?;
        Ok(Poll::Ready(StandaloneOpenedConnection {
            transport: Some(transport),
            pin,
            _process: &mut self.process,
        }))
    }

    /// Close listener admission and return the still-root-owning process state.
    /// Any opened/serving connection borrows this owner and therefore must have
    /// been closed and dropped before this method can be called.
    #[must_use]
    pub fn close_listener(self) -> StandaloneProcessOwner {
        let Self { listener, process, .. } = self;
        drop(listener);
        process
    }

    fn poll_accept(&self) -> Result<Option<TcpStream>, StandalonePairingIoError> {
        let started = Instant::now();
        loop {
            match self.listener.accept() {
                Ok((stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        let _ = stream.shutdown(Shutdown::Both);
                        return Err(StandalonePairingIoError::NonLoopback);
                    }
                    return Ok(Some(stream));
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    let elapsed = started.elapsed();
                    if elapsed >= self.accept_quantum {
                        return Ok(None);
                    }
                    std::thread::sleep(ACCEPT_SLEEP.min(self.accept_quantum - elapsed));
                }
                Err(error) => return Err(StandalonePairingIoError::Io(error)),
            }
        }
    }
}

impl fmt::Debug for StandaloneLoopbackOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneLoopbackOwner")
            .field("local_addr", &self.listener.local_addr().ok())
            .field("next_connection_sequence", &self.next_connection_sequence)
            .field("ready", &self.process.readiness())
            .finish_non_exhaustive()
    }
}
