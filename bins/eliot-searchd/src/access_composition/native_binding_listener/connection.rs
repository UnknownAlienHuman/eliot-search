//! Lifetime-bound opened and serving connection guards.

use core::fmt;
use std::task::Poll;

use crate::provider_composition::{
    CanonicalRecipeHost, CanonicalServingError, CanonicalServingLimits,
    CanonicalServingOwner, CanonicalTcpConnection,
};

use super::super::{
    BoundedStandaloneGrantIssuer, GrantValidationClock, NativeBindingPin,
    SessionBoundGrantAuthority, StandaloneGrantPolicySource, StandaloneGrantRecipeHost,
    StandaloneProcessOwner,
};

/// One opened typed transport whose lifetime is bounded by the process owner.
/// Fields are private: the transport cannot be detached or stored independently.
pub struct StandaloneOpenedConnection<'a> {
    pub(super) transport: Option<CanonicalTcpConnection>,
    pub(super) pin: NativeBindingPin,
    pub(super) _process: &'a mut StandaloneProcessOwner,
}

impl<'a> StandaloneOpenedConnection<'a> {
    /// Exact current binding pin retained for live authority composition.
    #[must_use]
    pub const fn pin(&self) -> &NativeBindingPin { &self.pin }

    /// Transfer this connection into the canonical bounded standalone serving
    /// owner while preserving its borrow of process/root lifetime.
    ///
    /// The query host is always wrapped by the original session-bound grant
    /// authority. No direct ungranted serving constructor is exposed from this
    /// opened native connection, so startup cannot accidentally bypass the
    /// issuer ledger or current durable policy checks.
    pub fn into_standalone_serving<P, E, T, H>(
        mut self,
        host: H,
        authority: SessionBoundGrantAuthority<P, BoundedStandaloneGrantIssuer<E, T>>,
        limits: CanonicalServingLimits,
    ) -> Result<
        StandaloneServingConnection<'a, StandaloneGrantRecipeHost<P, E, T, H>>,
        CanonicalServingError,
    >
    where
        P: StandaloneGrantPolicySource,
        T: GrantValidationClock,
        H: CanonicalRecipeHost,
    {
        let transport = self
            .transport
            .take()
            .ok_or(CanonicalServingError::Closed)?;
        let serving = CanonicalServingOwner::<
            StandaloneGrantRecipeHost<P, E, T, H>,
        >::new_standalone(transport, host, authority, limits)?;
        Ok(StandaloneServingConnection {
            serving,
            pin: self.pin,
            _process: self._process,
        })
    }

    /// Close without creating a recipe host, idempotently.
    pub fn close(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.close();
        }
    }
}

impl fmt::Debug for StandaloneOpenedConnection<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneOpenedConnection")
            .field("binding", &self.pin.record().binding_id)
            .field("open", &self.transport.is_some())
            .finish_non_exhaustive()
    }
}

/// Canonical serving owner whose transport and tasks cannot outlive the root.
pub struct StandaloneServingConnection<'a, H: CanonicalRecipeHost> {
    serving: CanonicalServingOwner<H>,
    pin: NativeBindingPin,
    _process: &'a mut StandaloneProcessOwner,
}

impl<H: CanonicalRecipeHost> StandaloneServingConnection<'_, H> {
    /// Exact binding pin for host-side live registration/policy checks.
    #[must_use]
    pub const fn pin(&self) -> &NativeBindingPin { &self.pin }

    /// Service one bounded input/work turn.
    pub fn tick(&mut self) -> Result<(), CanonicalServingError> { self.serving.tick() }

    /// Run and normally drain until the caller's stop condition becomes true.
    /// On error the connection is closed but retained tasks remain available
    /// through `poll_cleanup`; dropping is fail-stop, never cleanup evidence.
    pub fn run(
        &mut self,
        stop: impl FnMut() -> bool,
    ) -> Result<(), CanonicalServingError> {
        self.serving.run(stop)
    }

    /// Stop admission/output and signal retained task resources.
    pub fn close(&mut self) { self.serving.close(); }

    /// Advance at most one retained cleanup task after closure.
    pub fn poll_cleanup(&mut self) -> Result<Poll<()>, CanonicalServingError> {
        self.serving.poll_cleanup()
    }

    /// Queued/running/cleanup-pending task count.
    #[must_use]
    pub fn retained_tasks(&self) -> usize { self.serving.retained_tasks() }
}

impl<H: CanonicalRecipeHost> fmt::Debug for StandaloneServingConnection<'_, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StandaloneServingConnection")
            .field("binding", &self.pin.record().binding_id)
            .field("retained_tasks", &self.serving.retained_tasks())
            .finish_non_exhaustive()
    }
}
