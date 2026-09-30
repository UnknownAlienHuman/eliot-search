//! Loopback accept lifetime tied to the bootstrapped standalone process owner.
//!
//! This module binds only after durable bootstrap has produced
//! [`StandaloneBootstrapReady`]. Pairing code receives a restricted exact-I/O
//! view of the original socket: it cannot clone, buffer ahead or detach the
//! descriptor through this API. An opened or serving connection borrows the
//! process owner, so its transport, tasks and terminal cleanup cannot outlive the
//! control journal, admission snapshot or data-root lock.

mod connection;
mod io;

pub use connection::{StandaloneOpenedConnection, StandaloneServingConnection};
pub use io::{StandalonePairingIo, StandalonePairingIoError};

use core::fmt;
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::task::Poll;
use std::time::{Duration, Instant};

use search_contracts::{OpaqueId, protocol::PeerRole};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    BindingContext, PairingMachine, ProtocolError, ProtocolLimits, ServerNonce,
};

use super::{NativeTcpOpenError, StandaloneBootstrapReady, StandaloneProcessOwner};

const ACCEPT_SLEEP: Duration = Duration::from_millis(2);
const MAX_ACCEPT_QUANTUM: Duration = Duration::from_millis(250);

/// Pairing callback or native-open failure for one accepted socket.
#[derive(Debug)]
pub enum StandaloneAcceptError<E> {
    /// Listener or restricted pairing I/O failed.
    Io(StandalonePairingIoError),
    /// Caller-owned canonical pairing exchange refused the peer.
    Pairing(E),
    /// Published binding, credential, policy or typed profile opening failed.
    Open(NativeTcpOpenError),
}

impl<E: fmt::Display> fmt::Display for StandaloneAcceptError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => fmt::Display::fmt(error, formatter),
            Self::Pairing(error) => fmt::Display::fmt(error, formatter),
            Self::Open(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for StandaloneAcceptError<E> {}

impl<E> From<StandalonePairingIoError> for StandaloneAcceptError<E> {
    fn from(error: StandalonePairingIoError) -> Self { Self::Io(error) }
}

impl<E> From<NativeTcpOpenError> for StandaloneAcceptError<E> {
    fn from(error: NativeTcpOpenError) -> Self { Self::Open(error) }
}

/// Completed mutual pairing material for the same original accepted socket.
///
/// Construction verifies that the binding and completed ceremony correspond.
/// It does not validate current durable registration, credential presence,
/// policy, source scope or recipe authority; `StandaloneProcessOwner` does that
/// while opening the typed transport.
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
    process: StandaloneProcessOwner,
    next_connection_sequence: u64,
    accept_quantum: Duration,
}

impl StandaloneLoopbackOwner {
    /// Bind an IPv4 loopback listener after process bootstrap.
    ///
    /// Port zero is permitted for an explicitly supervised ephemeral endpoint.
    /// The finite poll quantum is not a request deadline; each accepted socket
    /// receives its own caller-supplied `OperationContext` before pairing begins.
    pub fn bind(
        process: StandaloneProcessOwner,
        port: u16,
        accept_quantum: Duration,
    ) -> Result<Self, StandalonePairingIoError> {
        if accept_quantum.is_zero() || accept_quantum > MAX_ACCEPT_QUANTUM {
            return Err(StandalonePairingIoError::InvalidConfiguration);
        }
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
            process,
            next_connection_sequence: 0,
            accept_quantum,
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

    /// Poll one accepted socket, run caller-owned canonical mutual pairing over
    /// its restricted original descriptor, then perform native typed opening.
    ///
    /// `pair` must consume only the exact bytes it defines and return a completed
    /// ceremony for this socket. It cannot access or detach the `TcpStream` through
    /// this API. `Poll::Pending` means no connection arrived during the configured
    /// accept quantum; no setup deadline or sequence was consumed.
    #[allow(clippy::too_many_arguments)]
    pub fn poll_open<C, E, F>(
        &mut self,
        limits: ProtocolLimits,
        boot_id: &OpaqueId,
        context: &OperationContext<C>,
        pair: F,
    ) -> Result<Poll<StandaloneOpenedConnection<'_>>, StandaloneAcceptError<E>>
    where
        C: CancellationProbe + Clone,
        F: FnOnce(
            &mut StandalonePairingIo<'_, C>,
            u64,
        ) -> Result<CompletedStandalonePairing, E>,
    {
        let Some(mut stream) = self.poll_accept()? else {
            return Ok(Poll::Pending);
        };
        let sequence = self
            .next_connection_sequence
            .checked_add(1)
            .ok_or(StandalonePairingIoError::ConnectionSequenceExhausted)?;
        self.next_connection_sequence = sequence;

        let mut io = StandalonePairingIo::new(&mut stream, context)?;
        let paired = pair(&mut io, sequence).map_err(StandaloneAcceptError::Pairing)?;
        let remaining = io.remaining_context()?;
        drop(io);

        let (transport, pin) = self.process.open_tcp(
            stream,
            paired.binding,
            paired.ceremony,
            paired.server_nonce,
            limits,
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
