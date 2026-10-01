//! Loopback accept lifetime tied to the bootstrapped standalone process owner.
//!
//! This module binds only after durable bootstrap has produced
//! [`StandaloneBootstrapReady`]. Pairing uses the canonical bounded prelude over
//! a restricted exact-I/O view of the original socket: it cannot clone, buffer
//! ahead or detach the descriptor. An opened or serving connection borrows the
//! process owner, so its transport, tasks and terminal cleanup cannot outlive the
//! control journal, admission snapshot or data-root lock.

mod connection;
mod endpoint;
mod io;
mod pairing;

pub use connection::{StandaloneOpenedConnection, StandaloneServingConnection};
pub use endpoint::{StandaloneEndpointAdvertisement, StandaloneEndpointError};
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

/// Listener bind/configuration or authenticated endpoint-publication failure.
#[derive(Debug)]
pub enum StandaloneBindError {
    /// Loopback listener, limits or polling configuration was invalid.
    Listener(StandalonePairingIoError),
    /// Signed descriptor publication failed; listener admission never opened.
    Endpoint(StandaloneEndpointError),
}

impl fmt::Display for StandaloneBindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listener(error) => fmt::Display::fmt(error, formatter),
            Self::Endpoint(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for StandaloneBindError {}

impl From<StandalonePairingIoError> for StandaloneBindError {
    fn from(error: StandalonePairingIoError) -> Self { Self::Listener(error) }
}

impl From<StandaloneEndpointError> for StandaloneBindError {
    fn from(error: StandaloneEndpointError) -> Self { Self::Endpoint(error) }
}

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

/// Listener, signed endpoint publication and root-owned process state.
///
/// The listener is created only after registration bootstrap/finalization
/// succeeded. Shutdown closes admission, removes the exact authenticated
/// descriptor and only then returns or drops the process/root owner.
pub struct StandaloneLoopbackOwner {
    listener: Option<TcpListener>,
    endpoint: Option<endpoint::PublishedNativeEndpoint>,
    pairing: BindingSession,
    process: Option<StandaloneProcessOwner>,
    next_connection_sequence: u64,
    accept_quantum: Duration,
    local_protocols: ProtocolRange,
    limits: ProtocolLimits,
}

impl StandaloneLoopbackOwner {
    /// Bind an IPv4 loopback listener and publish its authenticated descriptor.
    ///
    /// Port zero is permitted for an explicitly supervised ephemeral endpoint;
    /// the actual selected port is signed into `runtime/native-endpoint.v1`.
    /// The finite poll quantum is not a request deadline. One caller-supplied
    /// setup context covers exact credential resolution, descriptor publication
    /// and readback. The protocol range and limits are fixed for this listener
    /// lifetime.
    pub fn bind<C>(
        process: StandaloneProcessOwner,
        port: u16,
        accept_quantum: Duration,
        local_protocols: ProtocolRange,
        limits: ProtocolLimits,
        advertisement: &StandaloneEndpointAdvertisement,
        context: &OperationContext<C>,
    ) -> Result<Self, StandaloneBindError>
    where
        C: CancellationProbe + Clone,
    {
        if accept_quantum.is_zero() || accept_quantum > MAX_ACCEPT_QUANTUM {
            return Err(StandalonePairingIoError::InvalidConfiguration.into());
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
        let address = listener
            .local_addr()
            .map_err(StandalonePairingIoError::Io)?;
        if !address.ip().is_loopback() {
            return Err(StandalonePairingIoError::NonLoopback.into());
        }
        listener
            .set_nonblocking(true)
            .map_err(StandalonePairingIoError::Io)?;
        let endpoint = match endpoint::PublishedNativeEndpoint::publish(
            &process,
            address,
            advertisement,
            local_protocols,
            context,
        ) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                if error.requires_process_retention() {
                    std::mem::forget(process);
                }
                return Err(error.into());
            }
        };
        Ok(Self {
            listener: Some(listener),
            endpoint: Some(endpoint),
            pairing,
            process: Some(process),
            next_connection_sequence: 0,
            accept_quantum,
            local_protocols,
            limits,
        })
    }

    /// Actual bound address; contains no credential or source metadata.
    pub fn local_addr(&self) -> Result<SocketAddr, StandalonePairingIoError> {
        self.listener
            .as_ref()
            .ok_or(StandalonePairingIoError::InvalidConfiguration)?
            .local_addr()
            .map_err(StandalonePairingIoError::Io)
    }

    /// Executed bootstrap evidence retained by the underlying process owner.
    #[must_use]
    pub fn readiness(&self) -> &StandaloneBootstrapReady {
        self.process
            .as_ref()
            .expect("standalone process owner is present while listener is usable")
            .readiness()
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

        let process = self
            .process
            .as_ref()
            .ok_or(StandalonePairingIoError::InvalidConfiguration)?;
        let record = process.readiness().record().clone();
        let expected = process.readiness().expectation().clone();
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

        let process = self
            .process
            .as_mut()
            .ok_or(StandalonePairingIoError::InvalidConfiguration)?;
        let (transport, pin) = process.open_tcp(
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
            _process: process,
        }))
    }

    /// Close listener admission, remove the exact descriptor and return the
    /// still-root-owning process state.
    ///
    /// Any opened/serving connection borrows this owner and therefore must have
    /// been closed and dropped before this method can be called. If descriptor
    /// removal fails, drop retries it and retains the process/root lock in a
    /// fail-stop leak rather than releasing ownership behind a stale endpoint.
    pub fn close_listener(
        mut self,
    ) -> Result<StandaloneProcessOwner, StandaloneEndpointError> {
        drop(self.listener.take());
        if let Some(endpoint) = self.endpoint.as_mut() {
            endpoint.remove()?;
        }
        drop(self.endpoint.take());
        Ok(self
            .process
            .take()
            .expect("standalone process owner is present during explicit close"))
    }

    fn poll_accept(&self) -> Result<Option<TcpStream>, StandalonePairingIoError> {
        let listener = self
            .listener
            .as_ref()
            .ok_or(StandalonePairingIoError::InvalidConfiguration)?;
        let started = Instant::now();
        loop {
            match listener.accept() {
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

impl Drop for StandaloneLoopbackOwner {
    fn drop(&mut self) {
        drop(self.listener.take());
        let cleanup_failed = self
            .endpoint
            .as_mut()
            .is_some_and(|endpoint| endpoint.remove().is_err());
        if cleanup_failed {
            // Releasing the root while this exact descriptor may remain would
            // let a successor race stale discovery. Preserve fail-stop ownership
            // for the rest of this process instead; normal shutdown returns an
            // explicit error and the OS releases exclusion when the process exits.
            if let Some(process) = self.process.take() {
                std::mem::forget(process);
            }
        } else {
            drop(self.endpoint.take());
        }
    }
}

impl fmt::Debug for StandaloneLoopbackOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneLoopbackOwner")
            .field(
                "local_addr",
                &self
                    .listener
                    .as_ref()
                    .and_then(|listener| listener.local_addr().ok()),
            )
            .field("next_connection_sequence", &self.next_connection_sequence)
            .field(
                "ready",
                &self.process.as_ref().map(StandaloneProcessOwner::readiness),
            )
            .finish_non_exhaustive()
    }
}
