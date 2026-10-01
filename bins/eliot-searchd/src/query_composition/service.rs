//! Coherent canonical query host and its binding-visible capability descriptor.
//!
//! Construction consumes the recipe registry after capability projection. The
//! descriptor and dispatch table therefore cannot diverge through a later handler
//! insertion/removal. Listener/readiness publication remains daemon startup work;
//! this value alone does not claim that an endpoint is accepting requests.

use search_contracts::SearchProviderCapabilityDescriptor;
use search_provider_protocol::BindingContext;

use super::registry::{
    CanonicalCapabilityProjectionError, CanonicalRecipeRegistry,
};
use crate::query_serving_composition::{
    CanonicalQueryAuthorityOwner, CanonicalQueryHost,
};

/// One immutable capability projection paired with its exact serving host.
pub(crate) struct CanonicalRegisteredQueryService<O> {
    host: CanonicalQueryHost<O, CanonicalRecipeRegistry>,
    capability: SearchProviderCapabilityDescriptor,
}

impl<O> CanonicalRegisteredQueryService<O>
where
    O: CanonicalQueryAuthorityOwner,
{
    /// Compose one authority owner and one closed registry, then project the
    /// authoritative descriptor for the same authenticated binding.
    ///
    /// The registry is consumed only after every advertised recipe is proven to
    /// have a live handler. A projection error returns no host and no descriptor.
    pub(crate) fn new(
        authority: O,
        registry: CanonicalRecipeRegistry,
        authoritative_capability: &SearchProviderCapabilityDescriptor,
        binding: &BindingContext,
    ) -> Result<Self, CanonicalCapabilityProjectionError> {
        let capability = registry.project_capability(authoritative_capability, binding)?;
        let host = CanonicalQueryHost::new(authority, registry);
        Ok(Self { host, capability })
    }

    /// Exact descriptor that may be published for this binding.
    #[must_use]
    pub(crate) const fn capability(&self) -> &SearchProviderCapabilityDescriptor {
        &self.capability
    }

    /// Mutable request-serving host; its closed registry cannot be replaced.
    #[must_use]
    pub(crate) fn host_mut(
        &mut self,
    ) -> &mut CanonicalQueryHost<O, CanonicalRecipeRegistry> {
        &mut self.host
    }

    /// Transfer the coherent pair into the connection/readiness owner.
    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (
        CanonicalQueryHost<O, CanonicalRecipeRegistry>,
        SearchProviderCapabilityDescriptor,
    ) {
        (self.host, self.capability)
    }
}
