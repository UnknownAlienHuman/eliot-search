//! Native registration validation spans the original socket's profile handshake.

use std::net::{Shutdown, TcpStream};

use search_contracts::{OpaqueId, protocol::PeerRole};
use search_control_redb::{ControlSnapshotPublisher, PersistentControlJournal};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{BindingContext, MonotonicMillis, PairingMachine, ProtocolLimits, ServerNonce, TransportPeer};

use crate::access_composition::{GrantUseError, NativeGrantPolicyError, StandalonePolicyState};
use crate::provider_composition::{CanonicalProviderConnection, CanonicalTcpConnection, CanonicalTcpError, monotonic_millis};
use super::{NativeBindingError, NativeBindingExpectation, NativeBindingPin, NativePairingCredentialError, SystemGrantClock, begin, check};

/// Preserve native read/clock failures separately from actual handshake I/O.
/// Errors may follow acknowledgement output; they never imply peer receipt or
/// permission to reuse the socket. No record, key or peer text is rendered.
#[derive(Debug)]
pub enum NativeTcpOpenError {
    /// The required current-user pairing credential could not be resolved.
    Credential(NativePairingCredentialError),
    /// Current binding or original pairing/key did not validate.
    Binding(NativeBindingError),
    /// Coherent registration, policy state, boot or lifetime did not validate.
    Policy(NativeGrantPolicyError),
    /// The original socket's authenticated profile exchange failed.
    Transport(CanonicalTcpError),
}

impl std::fmt::Display for NativeTcpOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credential(error) => std::fmt::Display::fmt(error, f),
            Self::Binding(error) => std::fmt::Display::fmt(error, f),
            Self::Policy(error) => std::fmt::Display::fmt(error, f),
            Self::Transport(error) => std::fmt::Display::fmt(error, f),
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

impl CanonicalProviderConnection {
    /// Open a published standalone binding and negotiate its original TCP socket.
    ///
    /// One finite handoff deadline starts before native opening. Binding/policy
    /// lifetimes may shorten it; lookup, key verification, partial offer reads,
    /// acknowledgement writes and final validation never get another budget.
    /// The native cancellation capability is observed around actual setup I/O.
    ///
    /// The exact pin and coherent policy pair are revalidated before waiting,
    /// immediately before acknowledgement, and after output. An old boot, absent
    /// or terminal policy, changed binding or expired lifetime refuses handoff.
    /// Neither an invented RequestGuard nor a new grant/issuer is required.
    ///
    /// Caller holds the real native root/binding/policy mutation lock throughout.
    /// Expected peer/profile/disclosure inputs must already be resolved. The
    /// exact generation key is loaded from Windows Credential Manager here, not
    /// accepted as a caller-selected key. Missing credentials are never created.
    /// The ceremony must be complete on THIS socket, with no other
    /// reader or unread buffered bytes. Registration and dependent publication
    /// must already have finished. This does not create a listener or authorize
    /// any recipe: serving must retain the returned pin and revalidate live access.
    ///
    /// # Errors
    /// Any refusal, expiry, I/O failure or unwind closes the owned socket and
    /// drops the key/session. A late failure may follow acknowledgement bytes;
    /// no retry, rollback claim or legacy repair frame is attempted.
    #[allow(clippy::too_many_arguments)]
    pub fn open_standalone_tcp<C: CancellationProbe + Clone>(
        stream: TcpStream,
        binding: BindingContext,
        ceremony: PairingMachine,
        server_nonce: ServerNonce,
        limits: ProtocolLimits,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        expected: &NativeBindingExpectation,
        boot_id: &OpaqueId,
        clock: &mut SystemGrantClock,
        context: &OperationContext<C>,
    ) -> Result<(CanonicalTcpConnection, NativeBindingPin), NativeTcpOpenError> {
        // Own shutdown even if native validation fails before the TCP owner exists.
        let mut socket = SocketHandoff(Some(stream));
        let (started, deadline) = begin(context)?;
        if binding.role() != PeerRole::StandaloneCli {
            return Err(NativeBindingError::Unavailable.into());
        }
        let credential_context = remaining_context(context, started, deadline)?;
        let peer = TransportPeer {
            role: binding.role(), incarnation: binding.incarnation(), binding: binding.binding_id(),
        };
        let key = expected.load_pairing_key(&peer, &credential_context)?;
        check(context, started, deadline)?;
        let native_context = remaining_context(context, started, deadline)?;
        let (connection, pin) = Self::open_published(
            binding, ceremony, key, server_nonce, limits, journal, publisher,
            expected, clock, &native_context,
        )?;
        check(context, started, deadline)?;
        let stream = socket.0.take().ok_or(CanonicalTcpError::Closed)?;
        let transport = connection.into_tcp_authorized(
            stream, started, deadline, Some(context.cancellation()), |session| {
                let binding = session.binding_context();
                let read_context = remaining_context(context, started, deadline)?;
                let (registration, binding_expiry) = pin.read_standalone_registration(
                    &binding, journal, publisher, clock, &read_context,
                )?;
                let (_, policy) = registration.records().ok_or(NativeBindingError::Unavailable)?;
                if policy.state != StandalonePolicyState::Active
                    || &policy.policy.issued_boot_id != boot_id
                {
                    return Err(NativeGrantPolicyError::Grant(GrantUseError::PolicyUnavailable).into());
                }
                let policy_expiry = clock.check_policy_window(&policy.issued_at, policy.expires_at.as_ref())
                    .map_err(NativeBindingError::from)?;
                check(context, started, deadline)?;
                let until = binding_expiry.map_or(deadline, |end| deadline.min(end));
                Ok::<_, NativeTcpOpenError>(policy_expiry.map_or(until, |end| until.min(end)))
            },
        )?;
        Ok((transport, pin))
    }
}

fn remaining_context<C: CancellationProbe + Clone>(
    context: &OperationContext<C>,
    started: MonotonicMillis,
    deadline: MonotonicMillis,
) -> Result<OperationContext<C>, NativeBindingError> {
    check(context, started, deadline)?;
    let remaining = deadline.get().checked_sub(monotonic_millis().get())
        .filter(|left| *left > 0).ok_or(NativeBindingError::Interrupted)?;
    OperationContext::new(
        context.request_id(), remaining, context.cancellation().clone(), context.budget_ref().clone(),
    ).map_err(|_| NativeBindingError::Interrupted)
}

// The original descriptor is transferred once, not cloned. Once taken, the TCP
// owner is responsible for shutdown; before that, even an unwind closes here.
struct SocketHandoff(Option<TcpStream>);
impl Drop for SocketHandoff {
    fn drop(&mut self) {
        if let Some(stream) = &self.0 { let _ = stream.shutdown(Shutdown::Both); }
    }
}
