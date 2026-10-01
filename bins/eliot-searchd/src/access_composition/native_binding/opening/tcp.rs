//! Native registration validation spans the original local stream's profile handshake.

use std::net::TcpStream;

use search_contracts::{OpaqueId, protocol::PeerRole};
use search_control_redb::{ControlSnapshotPublisher, PersistentControlJournal};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    BindingContext, BoundSession, MonotonicMillis, PairingMachine, ProtocolLimits,
    ServerNonce, TransportPeer,
};

use crate::access_composition::{
    GrantUseError, NativeGrantPolicyError, StandaloneBootstrapReady, StandalonePolicyState,
};
use crate::provider_composition::{
    CanonicalProviderConnection, CanonicalTcpConnection, CanonicalTcpError, LocalByteStream,
    monotonic_millis,
};
use super::{
    BindingConnectionRegistry, BindingConnectionRegistryError, NativeBindingError,
    NativeBindingPin, NativePairingCredentialError, SystemGrantClock, begin, check,
};

/// Preserve native read/clock failures separately from actual local-stream I/O.
/// Errors may follow acknowledgement output; they never imply peer receipt or
/// permission to reuse the stream. No record, key or peer text is rendered.
#[derive(Debug)]
pub enum NativeTcpOpenError {
    /// The required current-user pairing credential could not be resolved.
    Credential(NativePairingCredentialError),
    /// Current binding or original pairing/key did not validate.
    Binding(NativeBindingError),
    /// Coherent registration, policy state, boot or lifetime did not validate.
    Policy(NativeGrantPolicyError),
    /// The authenticated local transport profile exchange failed.
    Transport(CanonicalTcpError),
    /// The verified session could not be retained by the finite binding registry.
    Registry(BindingConnectionRegistryError),
}

impl std::fmt::Display for NativeTcpOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credential(error) => std::fmt::Display::fmt(error, formatter),
            Self::Binding(error) => std::fmt::Display::fmt(error, formatter),
            Self::Policy(error) => std::fmt::Display::fmt(error, formatter),
            Self::Transport(error) => std::fmt::Display::fmt(error, formatter),
            Self::Registry(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}
impl std::error::Error for NativeTcpOpenError {}
impl From<NativePairingCredentialError> for NativeTcpOpenError {
    fn from(error: NativePairingCredentialError) -> Self { Self::Credential(error) }
}
impl From<NativeBindingError> for NativeTcpOpenError {
    fn from(error: NativeBindingError) -> Self { Self::Binding(error) }
}
impl From<NativeGrantPolicyError> for NativeTcpOpenError {
    fn from(error: NativeGrantPolicyError) -> Self { Self::Policy(error) }
}
impl From<CanonicalTcpError> for NativeTcpOpenError {
    fn from(error: CanonicalTcpError) -> Self { Self::Transport(error) }
}
impl From<BindingConnectionRegistryError> for NativeTcpOpenError {
    fn from(error: BindingConnectionRegistryError) -> Self { Self::Registry(error) }
}

impl CanonicalProviderConnection {
    /// Open a finalized standalone binding over the compatibility TCP stream.
    ///
    /// Loopback validation remains inside `into_tcp_authorized`. Final product
    /// startup must use the crate-private local-stream path with the canonical
    /// installation-scoped named-pipe adapter and must not fall back here.
    #[allow(clippy::too_many_arguments)]
    pub fn open_standalone_tcp<C: CancellationProbe + Clone>(
        stream: TcpStream,
        binding: BindingContext,
        ceremony: PairingMachine,
        server_nonce: ServerNonce,
        limits: ProtocolLimits,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        connections: &mut BindingConnectionRegistry,
        ready: &StandaloneBootstrapReady,
        boot_id: &OpaqueId,
        clock: &mut SystemGrantClock,
        context: &OperationContext<C>,
    ) -> Result<(CanonicalTcpConnection, NativeBindingPin), NativeTcpOpenError> {
        let PreparedNativeOpen {
            connection,
            pin,
            started,
            deadline,
        } = prepare_native_open(
            binding,
            ceremony,
            server_nonce,
            limits,
            journal,
            publisher,
            ready,
            clock,
            context,
        )?;
        let transport = connection.into_tcp_authorized(
            stream,
            started,
            deadline,
            Some(context.cancellation()),
            |session| {
                authorize_profile(
                    session,
                    &pin,
                    journal,
                    publisher,
                    boot_id,
                    clock,
                    context,
                    started,
                    deadline,
                )
            },
        )?;
        connections.register(&pin, &transport)?;
        Ok((transport, pin))
    }

    /// Open a finalized standalone binding over one already-accepted local stream.
    ///
    /// The stream is consumed exactly once and dropped on every refusal or unwind.
    /// Endpoint selection and acceptance happen in the platform adapter; this
    /// method performs no name scan, reconnect, TCP fallback or authority inference.
    /// Exact-generation Credential Manager resolution, current registration,
    /// policy lifetime, profile proof and registry retention remain identical to
    /// the compatibility TCP path.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_standalone_local<C, S>(
        stream: S,
        binding: BindingContext,
        ceremony: PairingMachine,
        server_nonce: ServerNonce,
        limits: ProtocolLimits,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        connections: &mut BindingConnectionRegistry,
        ready: &StandaloneBootstrapReady,
        boot_id: &OpaqueId,
        clock: &mut SystemGrantClock,
        context: &OperationContext<C>,
    ) -> Result<(CanonicalTcpConnection, NativeBindingPin), NativeTcpOpenError>
    where
        C: CancellationProbe + Clone,
        S: LocalByteStream + 'static,
    {
        let PreparedNativeOpen {
            connection,
            pin,
            started,
            deadline,
        } = prepare_native_open(
            binding,
            ceremony,
            server_nonce,
            limits,
            journal,
            publisher,
            ready,
            clock,
            context,
        )?;
        let transport = connection.into_local_stream_authorized(
            stream,
            started,
            deadline,
            Some(context.cancellation()),
            |session| {
                authorize_profile(
                    session,
                    &pin,
                    journal,
                    publisher,
                    boot_id,
                    clock,
                    context,
                    started,
                    deadline,
                )
            },
        )?;
        connections.register(&pin, &transport)?;
        Ok((transport, pin))
    }
}

struct PreparedNativeOpen {
    connection: CanonicalProviderConnection,
    pin: NativeBindingPin,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
}

#[allow(clippy::too_many_arguments)]
fn prepare_native_open<C: CancellationProbe + Clone>(
    binding: BindingContext,
    ceremony: PairingMachine,
    server_nonce: ServerNonce,
    limits: ProtocolLimits,
    journal: &PersistentControlJournal,
    publisher: &ControlSnapshotPublisher,
    ready: &StandaloneBootstrapReady,
    clock: &mut SystemGrantClock,
    context: &OperationContext<C>,
) -> Result<PreparedNativeOpen, NativeTcpOpenError> {
    let (started, deadline) = begin(context)?;
    if binding.role() != PeerRole::StandaloneCli {
        return Err(NativeBindingError::Unavailable.into());
    }
    ready.validate_context(journal, &binding)?;
    let expected = ready.expectation();
    let credential_context = remaining_context(context, started, deadline)?;
    let peer = TransportPeer {
        role: binding.role(),
        incarnation: binding.incarnation(),
        binding: binding.binding_id(),
    };
    let key = expected.load_pairing_key(&peer, &credential_context)?;
    check(context, started, deadline)?;
    let native_context = remaining_context(context, started, deadline)?;
    let (connection, pin) = CanonicalProviderConnection::open_published(
        binding,
        ceremony,
        key,
        server_nonce,
        limits,
        journal,
        publisher,
        expected,
        clock,
        &native_context,
    )?;
    ready.validate_opened_record(journal, pin.record())?;
    check(context, started, deadline)?;
    Ok(PreparedNativeOpen {
        connection,
        pin,
        started,
        deadline,
    })
}

#[allow(clippy::too_many_arguments)]
fn authorize_profile<C: CancellationProbe + Clone>(
    session: &BoundSession,
    pin: &NativeBindingPin,
    journal: &PersistentControlJournal,
    publisher: &ControlSnapshotPublisher,
    boot_id: &OpaqueId,
    clock: &mut SystemGrantClock,
    context: &OperationContext<C>,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
) -> Result<MonotonicMillis, NativeTcpOpenError> {
    let binding = session.binding_context();
    let read_context = remaining_context(context, started, deadline)?;
    let (registration, binding_expiry) = pin.read_standalone_registration(
        &binding,
        journal,
        publisher,
        clock,
        &read_context,
    )?;
    let (_, policy) = registration.records().ok_or(NativeBindingError::Unavailable)?;
    if policy.state != StandalonePolicyState::Active
        || &policy.policy.issued_boot_id != boot_id
    {
        return Err(NativeGrantPolicyError::Grant(GrantUseError::PolicyUnavailable).into());
    }
    let policy_expiry = clock
        .check_policy_window(&policy.issued_at, policy.expires_at.as_ref())
        .map_err(NativeBindingError::from)?;
    check(context, started, deadline)?;
    let until = binding_expiry.map_or(deadline, |end| deadline.min(end));
    Ok(policy_expiry.map_or(until, |end| until.min(end)))
}

fn remaining_context<C: CancellationProbe + Clone>(
    context: &OperationContext<C>,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
) -> Result<OperationContext<C>, NativeBindingError> {
    check(context, started, deadline)?;
    let remaining = deadline
        .get()
        .checked_sub(monotonic_millis().get())
        .filter(|left| *left > 0)
        .ok_or(NativeBindingError::Interrupted)?;
    OperationContext::new(
        context.request_id(),
        remaining,
        context.cancellation().clone(),
        context.budget_ref().clone(),
    )
    .map_err(|_| NativeBindingError::Interrupted)
}
