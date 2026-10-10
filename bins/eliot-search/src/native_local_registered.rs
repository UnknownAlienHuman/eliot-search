//! Trusted native registration resolution for the local provider transport.
//!
//! The final Windows client path derives one per-installation endpoint name and
//! one immutable Credential Manager locator exclusively from independently
//! trusted registration coordinates. It reads no endpoint file, accepts no host,
//! port, pipe prefix or caller-selected credential name, and grants no authority
//! merely because a local endpoint or credential exists.

#![allow(dead_code)]

use core::fmt;
use std::time::{Duration, Instant};

use search_contracts::{
    BindingId, Blake3Digest32, InstallationId, InstallationIncarnationId,
    NonZeroRevision, OpaqueRef, ProtocolRange, ProtocolVersion,
    protocol::PeerRole,
};
use search_ports::{CancellationProbe, OperationContext};
use search_provider_protocol::{
    BindingKey, NativeEndpointNameV1, ProtocolError,
    provider_pairing_credential_locator_material,
};

use crate::native_registered::NativePairingKeySource;
use crate::provider_client::{NativeClientBinding, TypedClientError};

const NATIVE_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

/// Independently trusted coordinates for one standalone local registration.
///
/// Every field comes from the installation/configuration owner. None is read
/// from the pipe namespace, filesystem, peer hello or Credential Manager record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeLocalRegistration {
    installation_id: InstallationId,
    installation_incarnation_id: InstallationIncarnationId,
    binding_id: BindingId,
    peer_identity_digest: Blake3Digest32,
    pairing_generation: NonZeroRevision,
    pairing_proof_ref: OpaqueRef,
    requested_capability_digest: Option<Blake3Digest32>,
}

impl NativeLocalRegistration {
    /// Retain one exact trusted standalone registration.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub(crate) const fn new(
        installation_id: InstallationId,
        installation_incarnation_id: InstallationIncarnationId,
        binding_id: BindingId,
        peer_identity_digest: Blake3Digest32,
        pairing_generation: NonZeroRevision,
        pairing_proof_ref: OpaqueRef,
        requested_capability_digest: Option<Blake3Digest32>,
    ) -> Self {
        Self {
            installation_id,
            installation_incarnation_id,
            binding_id,
            peer_identity_digest,
            pairing_generation,
            pairing_proof_ref,
            requested_capability_digest,
        }
    }

    /// Canonical installation-scoped local endpoint name.
    #[must_use]
    pub(crate) const fn endpoint_name(&self) -> NativeEndpointNameV1 {
        NativeEndpointNameV1::from_installation(self.installation_id)
    }
}

/// One-time resolved handoff from trusted registration to the platform connector.
///
/// The pairing key is non-clonable and zeroizes on drop. The context contains
/// only the diminishing remainder of the caller's original deadline. Possession
/// of this bundle is not a grant; mutual pairing and current daemon-side binding
/// validation remain mandatory.
pub(crate) struct ResolvedNativeLocal<C: CancellationProbe> {
    endpoint_name: NativeEndpointNameV1,
    binding: NativeClientBinding,
    key: BindingKey,
    context: OperationContext<C>,
}

impl<C: CancellationProbe> ResolvedNativeLocal<C> {
    /// Consume the handoff exactly once at the concrete local-socket connector.
    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (
        NativeEndpointNameV1,
        NativeClientBinding,
        BindingKey,
        OperationContext<C>,
    ) {
        (self.endpoint_name, self.binding, self.key, self.context)
    }
}

impl<C> fmt::Debug for ResolvedNativeLocal<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedNativeLocal")
            .field("endpoint_name", &self.endpoint_name)
            .field("binding", &self.binding)
            .field("key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Failure while resolving the exact local endpoint and pairing credential.
#[derive(Debug)]
pub(crate) enum ResolveNativeLocalError<E> {
    /// Canonical locator material or binding construction was invalid.
    Protocol(ProtocolError),
    /// The original operation was cancelled.
    Cancelled,
    /// The original operation deadline elapsed.
    DeadlineExpired,
    /// The platform credential owner failed while the budget remained valid.
    Credential(E),
    /// No exact registered-generation key exists.
    CredentialMissing,
    /// Trusted binding construction failed.
    Client(TypedClientError),
}

impl<E: fmt::Display> fmt::Display for ResolveNativeLocalError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => fmt::Display::fmt(error, formatter),
            Self::Cancelled => formatter.write_str("NATIVE_LOCAL_CANCELLED"),
            Self::DeadlineExpired => formatter.write_str("NATIVE_LOCAL_DEADLINE_EXPIRED"),
            Self::Credential(error) => fmt::Display::fmt(error, formatter),
            Self::CredentialMissing => formatter.write_str("NATIVE_LOCAL_CREDENTIAL_MISSING"),
            Self::Client(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for ResolveNativeLocalError<E> {}

impl<E> From<ProtocolError> for ResolveNativeLocalError<E> {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl<E> From<TypedClientError> for ResolveNativeLocalError<E> {
    fn from(error: TypedClientError) -> Self {
        Self::Client(error)
    }
}

/// Resolve one exact endpoint name, credential and native hello under one budget.
///
/// Credential absence never creates, replaces or adopts a key. The endpoint name
/// is derived, not discovered. There is no scan, TCP fallback, endpoint file,
/// token-file promotion, retry or deadline renewal.
pub(crate) fn resolve_registered_local<C, S>(
    registration: &NativeLocalRegistration,
    key_source: &mut S,
    context: &OperationContext<C>,
) -> Result<ResolvedNativeLocal<C>, ResolveNativeLocalError<S::Error>>
where
    C: CancellationProbe + Clone,
    S: NativePairingKeySource,
{
    let budget = AbsoluteBudget::new::<C, S::Error>(context)?;
    let locator_material = provider_pairing_credential_locator_material(
        registration.installation_id,
        registration.installation_incarnation_id,
        registration.binding_id,
        registration.peer_identity_digest,
        registration.pairing_generation,
        PeerRole::StandaloneCli,
    )?;
    let locator = *blake3::hash(&locator_material).as_bytes();
    let mut remaining = || {
        if context.cancellation().is_cancelled() {
            None
        } else {
            budget.optional_remaining()
        }
    };
    let key = match key_source.load_key(&locator, &mut remaining) {
        Ok(Some(key)) => key,
        Ok(None) => {
            budget.check(context)?;
            return Err(ResolveNativeLocalError::CredentialMissing);
        }
        Err(error) => {
            budget.check(context)?;
            return Err(ResolveNativeLocalError::Credential(error));
        }
    };
    budget.check(context)?;

    let range = ProtocolRange::new(NATIVE_PROTOCOL_VERSION, NATIVE_PROTOCOL_VERSION)
        .map_err(|_| ProtocolError::InvalidVersion)?;
    let binding = NativeClientBinding::new_trusted(
        registration.installation_id,
        registration.installation_incarnation_id,
        registration.binding_id,
        registration.pairing_proof_ref.clone(),
        range,
        registration.requested_capability_digest,
    )?;
    let context = budget.remaining_context(context)?;
    Ok(ResolvedNativeLocal {
        endpoint_name: registration.endpoint_name(),
        binding,
        key,
        context,
    })
}

struct AbsoluteBudget {
    deadline: Instant,
}

impl AbsoluteBudget {
    fn new<C: CancellationProbe, E>(
        context: &OperationContext<C>,
    ) -> Result<Self, ResolveNativeLocalError<E>> {
        if context.cancellation().is_cancelled() {
            return Err(ResolveNativeLocalError::Cancelled);
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(context.relative_deadline_ms().get()))
            .ok_or(ResolveNativeLocalError::DeadlineExpired)?;
        Ok(Self { deadline })
    }

    fn optional_remaining(&self) -> Option<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
    }

    fn check<C: CancellationProbe, E>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<Duration, ResolveNativeLocalError<E>> {
        if context.cancellation().is_cancelled() {
            return Err(ResolveNativeLocalError::Cancelled);
        }
        self.optional_remaining()
            .ok_or(ResolveNativeLocalError::DeadlineExpired)
    }

    fn remaining_context<C: CancellationProbe + Clone, E>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<OperationContext<C>, ResolveNativeLocalError<E>> {
        let remaining = self.check(context)?;
        let millis = u64::try_from(remaining.as_millis())
            .ok()
            .filter(|value| *value > 0)
            .ok_or(ResolveNativeLocalError::DeadlineExpired)?;
        OperationContext::new(
            context.request_id(),
            millis,
            context.cancellation().clone(),
            context.budget_ref().clone(),
        )
        .map_err(|_| ResolveNativeLocalError::DeadlineExpired)
    }
}
