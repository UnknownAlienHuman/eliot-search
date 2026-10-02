//! Coherent canonical query host and its binding-visible capability descriptor.
//!
//! Construction consumes the recipe registry after capability projection. The
//! descriptor and dispatch table therefore cannot diverge through a later handler
//! insertion/removal. Attachment consumes one opened native connection for the
//! exact same paired binding and always installs the session grant authority.

use std::task::Poll;

use search_contracts::SearchProviderCapabilityDescriptor;
use search_provider_protocol::BindingContext;

use super::registry::{
    CanonicalCapabilityProjectionError, CanonicalRecipeRegistry,
};
use crate::access_composition::{
    BoundedStandaloneGrantIssuer, GrantValidationClock, NativeBindingPin,
    SessionBoundGrantAuthority, StandaloneGrantPolicySource, StandaloneGrantRecipeHost,
    StandaloneOpenedConnection, StandaloneServingConnection,
};
use crate::provider_composition::{CanonicalServingError, CanonicalServingLimits};
use crate::query_serving_composition::{
    CanonicalQueryAuthorityOwner, CanonicalQueryHost,
};

/// One immutable capability projection paired with its exact serving host.
pub(crate) struct CanonicalRegisteredQueryService<O> {
    host: CanonicalQueryHost<O, CanonicalRecipeRegistry>,
    capability: SearchProviderCapabilityDescriptor,
    binding: BindingContext,
}

impl<O> CanonicalRegisteredQueryService<O>
where
    O: CanonicalQueryAuthorityOwner,
{
    /// Compose one authority owner and one closed registry, then project the
    /// authoritative descriptor for the exact pinned authenticated binding.
    ///
    /// The registry is consumed only after every advertised recipe is proven to
    /// have a live handler. A projection error returns no host and no descriptor.
    /// The pin is not treated as reusable authority; its current durable record
    /// still has to be revalidated by the owning request-serving composition.
    pub(crate) fn new(
        authority: O,
        registry: CanonicalRecipeRegistry,
        authoritative_capability: &SearchProviderCapabilityDescriptor,
        binding_pin: &NativeBindingPin,
    ) -> Result<Self, CanonicalCapabilityProjectionError> {
        let binding = binding_pin.binding_context();
        let capability = registry.project_capability(authoritative_capability, &binding)?;
        let host = CanonicalQueryHost::new(authority, registry);
        Ok(Self {
            host,
            capability,
            binding,
        })
    }

    /// Attach the coherent host/descriptor pair and the original issuer ledger
    /// to the exact opened session.
    ///
    /// A binding ID/incarnation match is insufficient: the complete authenticated
    /// `BindingContext`, including its verified pairing ceremony, must be equal.
    /// Mismatch drops the opened connection and publishes nothing. The opened
    /// connection exposes no direct ungranted serving path.
    pub(crate) fn into_serving<'a, P, E, T>(
        self,
        opened: StandaloneOpenedConnection<'a>,
        authority: SessionBoundGrantAuthority<P, BoundedStandaloneGrantIssuer<E, T>>,
        limits: CanonicalServingLimits,
    ) -> Result<CanonicalRegisteredServingConnection<'a, P, E, T, O>, CanonicalServingError>
    where
        P: StandaloneGrantPolicySource,
        T: GrantValidationClock,
    {
        let pin = opened.pin();
        if pin.binding_context() != self.binding
            || pin.record().binding_id != self.binding.binding_id()
            || pin.record().installation_incarnation_id != self.binding.incarnation()
        {
            return Err(CanonicalServingError::InvalidConfiguration);
        }
        let connection = opened.into_standalone_serving(self.host, authority, limits)?;
        Ok(CanonicalRegisteredServingConnection {
            connection,
            capability: self.capability,
        })
    }
}

/// Active native query connection retaining its exact published capability.
///
/// The descriptor cannot be detached from the closed handler registry or the
/// original standalone issuer ledger while the connection serves. Endpoint and
/// readiness owners may borrow it for publication, but request work and cleanup
/// remain owned by the same connection value.
pub(crate) struct CanonicalRegisteredServingConnection<'a, P, E, T, O>
where
    P: StandaloneGrantPolicySource,
    T: GrantValidationClock,
    O: CanonicalQueryAuthorityOwner,
{
    connection: StandaloneServingConnection<
        'a,
        StandaloneGrantRecipeHost<
            P,
            E,
            T,
            CanonicalQueryHost<O, CanonicalRecipeRegistry>,
        >,
    >,
    capability: SearchProviderCapabilityDescriptor,
}

impl<'a, P, E, T, O> CanonicalRegisteredServingConnection<'a, P, E, T, O>
where
    P: StandaloneGrantPolicySource,
    T: GrantValidationClock,
    O: CanonicalQueryAuthorityOwner,
{
    /// Exact binding-filtered descriptor backed by this connection's registry.
    #[must_use]
    pub(crate) const fn capability(&self) -> &SearchProviderCapabilityDescriptor {
        &self.capability
    }

    /// Current native binding pin retained by the underlying opened session.
    #[must_use]
    pub(crate) const fn pin(&self) -> &NativeBindingPin {
        self.connection.pin()
    }

    /// Service one bounded input/work turn.
    pub(crate) fn tick(&mut self) -> Result<(), CanonicalServingError> {
        self.connection.tick()
    }

    /// Run and normally drain until the caller's stop condition becomes true.
    pub(crate) fn run(
        &mut self,
        stop: impl FnMut() -> bool,
    ) -> Result<(), CanonicalServingError> {
        self.connection.run(stop)
    }

    /// Stop admission/output and signal retained task resources.
    pub(crate) fn close(&mut self) {
        self.connection.close();
    }

    /// Advance at most one retained cleanup task after closure.
    pub(crate) fn poll_cleanup(&mut self) -> Result<Poll<()>, CanonicalServingError> {
        self.connection.poll_cleanup()
    }

    /// Queued/running/cleanup-pending task count.
    #[must_use]
    pub(crate) fn retained_tasks(&self) -> usize {
        self.connection.retained_tasks()
    }
}
