//! Installation-scoped local-listener lifetime under the bootstrapped process owner.
//!
//! This owner is transport-neutral. A platform factory receives only the
//! canonical [`NativeEndpointNameV1`] derived from the finalized registration,
//! creates one nonblocking local listener, and returns accepted streams through
//! [`StandaloneLocalListener`]. Endpoint existence and ACLs authenticate nothing;
//! every stream still completes canonical mutual pairing and current native
//! registration/policy validation before it can serve work.

#![allow(dead_code)]

use core::fmt;
use std::io;
use std::task::Poll;
use std::time::{Duration, Instant};

use search_contracts::{OpaqueId, ProtocolRange, protocol::PeerRole};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    BindingSession, NativeEndpointNameV1, ProtocolLimits, TransportPeer,
};

use crate::provider_composition::LocalByteStream;

use super::{
    ACCEPT_SLEEP, MAX_ACCEPT_QUANTUM, StandaloneAcceptError,
    StandaloneOpenedConnection, StandalonePairingIo, StandalonePairingIoError,
    pairing,
};
use super::super::{
    NativeBindingError, StandaloneBootstrapReady, StandaloneProcessOwner,
};

/// Nonblocking platform listener for one canonical local endpoint.
///
/// Implementations own only listener/accept effects. They must bind exactly the
/// name supplied by [`StandaloneLocalOwner::bind`], configure accept as
/// nonblocking, return one original un-cloned stream, and perform no scan,
/// alternate-name retry, TCP fallback, framing or authentication decision.
pub(crate) trait StandaloneLocalListener {
    /// Accepted stream type transferred into the canonical pairing/session owner.
    type Stream: LocalByteStream + 'static;

    /// Accept one stream or return `WouldBlock` when none is ready.
    fn accept(&self) -> io::Result<Self::Stream>;
}

/// Listener construction or trusted registration refusal.
#[derive(Debug)]
pub(crate) enum StandaloneLocalBindError {
    /// Finalized registration did not match its independently trusted expectation.
    Registration(NativeBindingError),
    /// Listener factory or local polling configuration failed.
    Listener(StandalonePairingIoError),
}

impl fmt::Display for StandaloneLocalBindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registration(error) => fmt::Display::fmt(error, formatter),
            Self::Listener(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for StandaloneLocalBindError {}

impl From<NativeBindingError> for StandaloneLocalBindError {
    fn from(error: NativeBindingError) -> Self {
        Self::Registration(error)
    }
}

impl From<StandalonePairingIoError> for StandaloneLocalBindError {
    fn from(error: StandalonePairingIoError) -> Self {
        Self::Listener(error)
    }
}

/// Canonical local listener plus the root-owned process state it serves.
///
/// Field order is intentional: listener admission closes before the process and
/// data-root owner are dropped. An opened or serving connection borrows this
/// owner, so neither transport nor retained work can outlive that root lock.
pub(crate) struct StandaloneLocalOwner<L: StandaloneLocalListener> {
    listener: L,
    process: StandaloneProcessOwner,
    endpoint_name: NativeEndpointNameV1,
    pairing: BindingSession,
    next_connection_sequence: u64,
    accept_quantum: Duration,
    local_protocols: ProtocolRange,
    limits: ProtocolLimits,
}

impl<L: StandaloneLocalListener> StandaloneLocalOwner<L> {
    /// Derive the endpoint from finalized registration and construct one listener.
    ///
    /// The factory receives no path, host, port, remote prefix, credential or
    /// authority token. A successful bind is not readiness or authentication.
    /// Listener creation runs once; failure never retries with another name.
    pub(crate) fn bind(
        process: StandaloneProcessOwner,
        accept_quantum: Duration,
        local_protocols: ProtocolRange,
        limits: ProtocolLimits,
        create_listener: impl FnOnce(NativeEndpointNameV1) -> io::Result<L>,
    ) -> Result<Self, StandaloneLocalBindError> {
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
        let endpoint_name = trusted_endpoint_name(&process)?;
        let listener = create_listener(endpoint_name)
            .map_err(StandalonePairingIoError::Io)?;
        Ok(Self {
            listener,
            process,
            endpoint_name,
            pairing,
            next_connection_sequence: 0,
            accept_quantum,
            local_protocols,
            limits,
        })
    }

    /// Exact canonical endpoint name supplied to the platform listener factory.
    #[must_use]
    pub(crate) const fn endpoint_name(&self) -> NativeEndpointNameV1 {
        self.endpoint_name
    }

    /// Executed bootstrap evidence retained by the underlying process owner.
    #[must_use]
    pub(crate) const fn readiness(&self) -> &StandaloneBootstrapReady {
        self.process.readiness()
    }

    /// Poll one accepted stream through pairing and current native validation.
    ///
    /// `Pending` means no connection arrived during the finite accept quantum;
    /// it consumes no setup deadline or connection sequence. A failed accepted
    /// stream is dropped and is never retried or routed to TCP/legacy framing.
    pub(crate) fn poll_open<C: CancellationProbe + Clone>(
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

        let (transport, pin) = self.process.open_local(
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
    ///
    /// Any opened/serving connection must already be dropped because it borrows
    /// this owner. Destructuring drops the listener before returning the process.
    #[must_use]
    pub(crate) fn close_listener(self) -> StandaloneProcessOwner {
        let Self { listener, process, .. } = self;
        drop(listener);
        process
    }

    fn poll_accept(&self) -> Result<Option<L::Stream>, StandalonePairingIoError> {
        let started = Instant::now();
        loop {
            match self.listener.accept() {
                Ok(stream) => return Ok(Some(stream)),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
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

impl<L: StandaloneLocalListener> fmt::Debug for StandaloneLocalOwner<L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneLocalOwner")
            .field("endpoint_name", &self.endpoint_name)
            .field("next_connection_sequence", &self.next_connection_sequence)
            .field("ready", &self.process.readiness())
            .finish_non_exhaustive()
    }
}

fn trusted_endpoint_name(
    process: &StandaloneProcessOwner,
) -> Result<NativeEndpointNameV1, NativeBindingError> {
    let ready = process.readiness();
    let record = ready.record();
    let expected = ready.expectation();
    if record.peer_role != PeerRole::StandaloneCli {
        return Err(NativeBindingError::Unavailable);
    }
    let peer = TransportPeer {
        role: record.peer_role,
        incarnation: record.installation_incarnation_id,
        binding: record.binding_id,
    };
    expected.validate_registration(record, &peer)?;
    Ok(NativeEndpointNameV1::from_installation(
        expected.installation_id,
    ))
}
